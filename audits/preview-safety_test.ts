import { safeHttpUrl, publicAddress } from "../supabase/functions/_shared/safe-http.ts";
Deno.test("reject private, mapped, reserved and credentialed destinations", () => {
 for(const url of ["http://[::1]/","http://[fd00::1]/","http://[fe80::1]/","http://[::ffff:127.0.0.1]/","http://127.1/","http://2130706433/","http://10.1.2.3/","http://169.254.169.254/","http://192.168.1.2/","http://100.64.0.1/","http://0.0.0.0/","http://localhost/","http://service.internal/","https://user:password@example.com/","http://example.com:8080/"]) if(safeHttpUrl(url)) throw new Error(`Allowed unsafe URL: ${url}`);
 for(const address of ["::","::1","::ffff:10.0.0.1","fc00::1","fe80::1","224.0.0.1","255.255.255.255","192.0.2.1"]) if(publicAddress(address)) throw new Error(`Allowed unsafe address: ${address}`);
 if(!safeHttpUrl("https://example.com/page")||!publicAddress("1.1.1.1")||!publicAddress("2606:4700:4700::1111")) throw new Error("Rejected public destination");
});
