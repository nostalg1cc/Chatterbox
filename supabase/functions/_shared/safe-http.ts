import ipaddr from "npm:ipaddr.js@2.2.0";
import { lookup } from "node:dns/promises";

export function publicAddress(address: string): boolean {
  try {
    const parsed = ipaddr.process(address.replace(/^\[|\]$/g, ""));
    return parsed.range() === "unicast";
  } catch { return false; }
}

export function safeHttpUrl(value: string, base?: string): URL | null {
  try {
    const url = new URL(value, base);
    const host = url.hostname.toLowerCase().replace(/^\[|\]$/g, "");
    if (!["http:", "https:"].includes(url.protocol) || url.username || url.password
      || (url.port && !["80", "443"].includes(url.port))) return null;
    if (!host.includes(".") && !host.includes(":")) return null;
    if (/(^|\.)(localhost|local|internal|home\.arpa)$/.test(host)) return null;
    if (ipaddr.isValid(host) && !publicAddress(host)) return null;
    return url;
  } catch { return null; }
}

// Pin the actual socket to the validated DNS result, preserving TLS hostname
// verification. A second DNS lookup cannot redirect the request into a private network.
export async function fetchPublicDocument(url: URL, maxBytes: number, timeoutMs: number) {
  if (!safeHttpUrl(url.href)) throw new Error("Unsafe destination");
  const host = url.hostname.replace(/^\[|\]$/g, "");
  let connection: Deno.Conn | null = null;
  const controller = new AbortController();
  const timer = setTimeout(() => { controller.abort(); try { connection?.close(); } catch { /* closed */ } }, timeoutMs);
  try {
    const addresses = await Promise.race([
      lookup(host, { all: true }),
      new Promise<never>((_, reject) => controller.signal.addEventListener("abort", () => reject(new Error("DNS timed out")), { once: true })),
    ]);
    if (!addresses.length || addresses.some(({ address }) => !publicAddress(address))) throw new Error("Unsafe DNS destination");
    // The socket uses the checked numeric address; TLS verifies the original hostname.
    const tcp = await Deno.connect({ hostname: addresses[0].address, port: Number(url.port) || (url.protocol === "https:" ? 443 : 80), signal: controller.signal });
    connection = tcp;
    if (url.protocol === "https:") connection = await Deno.startTls(tcp, { hostname: host });
    const request = new TextEncoder().encode(`GET ${url.pathname}${url.search} HTTP/1.1\r\nHost: ${url.host}\r\nUser-Agent: Nitro Link Preview/1.0\r\nAccept: text/html,application/xhtml+xml\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n`);
    for (let written = 0; written < request.length;) written += await connection.write(request.subarray(written));
    let bytes = new Uint8Array(0), headerEnd = -1, status = 0;
    const headers: Record<string, string> = {};
    const buffer = new Uint8Array(16_384);
    while (true) {
      const count = await connection.read(buffer);
      if (count === null) break;
      if (bytes.length + count > maxBytes + 65_536) throw new Error("Page too large");
      const next = new Uint8Array(bytes.length + count); next.set(bytes); next.set(buffer.subarray(0,count),bytes.length); bytes = next;
      if (headerEnd === -1) {
        for (let i = 0; i <= bytes.length - 4; i++) if (bytes[i] === 13 && bytes[i+1] === 10 && bytes[i+2] === 13 && bytes[i+3] === 10) { headerEnd = i+4; break; }
        if (headerEnd === -1) { if (bytes.length > 16_384) throw new Error("Headers too large"); continue; }
        const lines = new TextDecoder().decode(bytes.subarray(0,headerEnd-4)).split("\r\n");
        status = Number(lines.shift()?.match(/^HTTP\/1\.[01] (\d{3}) /)?.[1]);
        if (!status || status < 200) throw new Error("Unsupported response");
        for (const line of lines) { const colon=line.indexOf(":"); if(colon>0) headers[line.slice(0,colon).toLowerCase()] = line.slice(colon+1).trim(); }
        if (status >= 300 && status < 400) return { status,headers,html: "" };
        if (Number(headers["content-length"] ?? 0) > maxBytes) throw new Error("Page too large");
        if (headers["content-encoding"] && headers["content-encoding"] !== "identity") throw new Error("Unsupported encoding");
      }
      if (headers["content-length"] && bytes.length-headerEnd >= Number(headers["content-length"])) break;
    }
    if (headerEnd === -1) throw new Error("Missing response headers");
    let body: Uint8Array = bytes.subarray(headerEnd);
    if (headers["transfer-encoding"]?.toLowerCase() === "chunked") body = decodeChunks(body, maxBytes);
    else if (headers["transfer-encoding"]) throw new Error("Unsupported transfer encoding");
    else if (headers["content-length"] && body.length !== Number(headers["content-length"])) throw new Error("Truncated response");
    if (body.length > maxBytes) throw new Error("Page too large");
    return { status,headers,html: new TextDecoder().decode(body) };
  } finally { clearTimeout(timer); try { connection?.close(); } catch { /* already closed */ } }
}

function decodeChunks(input: Uint8Array, maxBytes: number): Uint8Array {
  const chunks: Uint8Array[] = []; let offset=0,total=0;
  while(offset<input.length) {
    let end=offset;
    while(end<input.length-1 && !(input[end]===13 && input[end+1]===10)) end++;
    const line = new TextDecoder().decode(input.subarray(offset,end));
    if(!/^[0-9a-f]+(?:;.*)?$/i.test(line)) throw new Error("Invalid chunk");
    const size=parseInt(line,16); offset=end+2;
    if(size===0) { const result=new Uint8Array(total);let written=0;for(const chunk of chunks){result.set(chunk,written);written+=chunk.length;}return result; }
    total+=size;
    if(total>maxBytes || offset+size+2>input.length || input[offset+size]!==13 || input[offset+size+1]!==10) throw new Error("Invalid chunk size");
    chunks.push(input.subarray(offset,offset+size));offset+=size+2;
  }
  throw new Error("Truncated chunks");
}
