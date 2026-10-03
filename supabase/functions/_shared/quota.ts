export async function allowRequest(admin: any, key: string, action: string, limit: number, windowSeconds = 60): Promise<boolean> {
  const { data, error } = await admin.rpc("consume_edge_quota", {
    p_key: key, p_action: action, p_limit: limit, p_window_seconds: windowSeconds,
  });
  return !error && data === true;
}
