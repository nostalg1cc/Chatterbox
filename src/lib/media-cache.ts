const DB_NAME = "dislight-media-cache";
const STORE_NAME = "media";
const DB_VERSION = 1;
const THIRTY_DAYS_MS = 30 * 24 * 60 * 60 * 1000;
const HARD_CACHE_LIMIT_BYTES = 1024 * 1024 * 1024;
const MIN_CACHE_LIMIT_BYTES = 128 * 1024 * 1024;

export interface CachedMedia {
  key: string;
  userId: string;
  messageId: string;
  blob: Blob;
  mimeType: string;
  bytes: number;
  createdAt: number;
  expiresAt: number;
  lastAccessedAt: number;
}

export interface MediaCacheStats {
  entries: number;
  bytes: number;
  limitBytes: number;
}

let databasePromise: Promise<IDBDatabase> | null = null;

function cacheKey(userId: string, messageId: string) {
  return userId + ":" + messageId;
}

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed."));
  });
}

function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed."));
    transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted."));
  });
}

function openDatabase(): Promise<IDBDatabase> {
  if (databasePromise) return databasePromise;
  databasePromise = new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onupgradeneeded = () => {
      const database = request.result;
      const store = database.createObjectStore(STORE_NAME, { keyPath: "key" });
      store.createIndex("userId", "userId", { unique: false });
      store.createIndex("expiresAt", "expiresAt", { unique: false });
      store.createIndex("lastAccessedAt", "lastAccessedAt", { unique: false });
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("Local media cache is unavailable."));
  });
  return databasePromise;
}

async function allRecords() {
  const database = await openDatabase();
  const transaction = database.transaction(STORE_NAME, "readonly");
  return requestResult(transaction.objectStore(STORE_NAME).getAll()) as Promise<CachedMedia[]>;
}

async function cacheLimitBytes() {
  try {
    const estimate = await navigator.storage?.estimate();
    if (estimate?.quota) {
      return Math.max(
        MIN_CACHE_LIMIT_BYTES,
        Math.min(HARD_CACHE_LIMIT_BYTES, Math.floor(estimate.quota * 0.25))
      );
    }
  } catch {
    // Fall through to the hard limit when the browser does not expose an estimate.
  }
  return HARD_CACHE_LIMIT_BYTES;
}

async function enforceCacheLimit() {
  const records = await allRecords();
  const limit = await cacheLimitBytes();
  let total = records.reduce((sum, record) => sum + record.bytes, 0);
  if (total <= limit) return;

  records.sort((a, b) => a.lastAccessedAt - b.lastAccessedAt);
  const database = await openDatabase();
  const transaction = database.transaction(STORE_NAME, "readwrite");
  const store = transaction.objectStore(STORE_NAME);
  for (const record of records) {
    if (total <= limit) break;
    store.delete(record.key);
    total -= record.bytes;
  }
  await transactionDone(transaction);
}

export async function getCachedMedia(
  userId: string,
  messageId: string
): Promise<CachedMedia | null> {
  const database = await openDatabase();
  const transaction = database.transaction(STORE_NAME, "readwrite");
  const store = transaction.objectStore(STORE_NAME);
  const key = cacheKey(userId, messageId);
  const record = await requestResult(store.get(key)) as CachedMedia | undefined;
  if (!record) {
    await transactionDone(transaction);
    return null;
  }
  if (record.expiresAt <= Date.now()) {
    store.delete(key);
    await transactionDone(transaction);
    return null;
  }
  record.lastAccessedAt = Date.now();
  store.put(record);
  await transactionDone(transaction);
  return record;
}

export async function putCachedMedia({
  userId,
  messageId,
  blob,
  mimeType,
  createdAt,
}: {
  userId: string;
  messageId: string;
  blob: Blob;
  mimeType: string;
  createdAt: string;
}) {
  const createdTime = Date.parse(createdAt);
  const expiresAt = createdTime + THIRTY_DAYS_MS;
  if (!Number.isFinite(createdTime) || expiresAt <= Date.now()) return;

  const storedBlob =
    blob.type === mimeType ? blob : new Blob([blob], { type: mimeType });
  const database = await openDatabase();
  const transaction = database.transaction(STORE_NAME, "readwrite");
  transaction.objectStore(STORE_NAME).put({
    key: cacheKey(userId, messageId),
    userId,
    messageId,
    blob: storedBlob,
    mimeType,
    bytes: storedBlob.size,
    createdAt: createdTime,
    expiresAt,
    lastAccessedAt: Date.now(),
  } satisfies CachedMedia);
  await transactionDone(transaction);
  await enforceCacheLimit();
}

export async function deleteCachedMedia(userId: string, messageId: string) {
  const database = await openDatabase();
  const transaction = database.transaction(STORE_NAME, "readwrite");
  transaction.objectStore(STORE_NAME).delete(cacheKey(userId, messageId));
  await transactionDone(transaction);
}

export async function purgeExpiredMediaCache() {
  const records = await allRecords();
  const expired = records.filter((record) => record.expiresAt <= Date.now());
  if (expired.length === 0) {
    await enforceCacheLimit();
    return;
  }
  const database = await openDatabase();
  const transaction = database.transaction(STORE_NAME, "readwrite");
  const store = transaction.objectStore(STORE_NAME);
  for (const record of expired) store.delete(record.key);
  await transactionDone(transaction);
  await enforceCacheLimit();
}

export async function clearLocalMediaCache(userId: string) {
  const records = (await allRecords()).filter((record) => record.userId === userId);
  const database = await openDatabase();
  const transaction = database.transaction(STORE_NAME, "readwrite");
  const store = transaction.objectStore(STORE_NAME);
  for (const record of records) store.delete(record.key);
  await transactionDone(transaction);
}

export async function mediaCacheStats(userId: string): Promise<MediaCacheStats> {
  const records = (await allRecords()).filter(
    (record) => record.userId === userId && record.expiresAt > Date.now()
  );
  return {
    entries: records.length,
    bytes: records.reduce((sum, record) => sum + record.bytes, 0),
    limitBytes: await cacheLimitBytes(),
  };
}
