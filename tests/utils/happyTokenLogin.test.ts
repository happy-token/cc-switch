import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { describe, expect, it, vi } from "vitest";

const script = readFileSync(
  "src-tauri/src/commands/happy_token_login.js",
  "utf8",
).replace("__HAPPY_NONCE__", "nonce");

function setup(
  origin = "https://gateway.happy-token.cn",
  uid = "42",
  path = "/dashboard",
) {
  let tick: () => Promise<void> = async () => {};
  const assign = vi.fn();
  const fetch = vi.fn().mockResolvedValue({
    ok: true,
    json: async () => ({ success: true, data: { id: 42 } }),
  });
  const location = { origin, pathname: path, assign };
  runInNewContext(script, {
    window: { location },
    location,
    localStorage: { getItem: () => uid },
    fetch,
    setInterval: (fn: () => Promise<void>) => {
      tick = fn;
      return 1;
    },
    clearInterval: vi.fn(),
    AbortController,
    setTimeout,
    clearTimeout,
  });
  return { tick: () => tick(), assign, fetch };
}

describe("HappyToken login session bridge", () => {
  it("signals a verified user once and never puts credentials in the URL", async () => {
    const { tick, assign, fetch } = setup();
    await tick();
    await tick();
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(fetch).toHaveBeenCalledWith(
      "/api/user/self",
      expect.objectContaining({
        credentials: "same-origin",
        headers: { "New-Api-User": "42", Accept: "application/json" },
      }),
    );
    expect(assign).toHaveBeenCalledOnce();
    const callback = new URL(assign.mock.calls[0][0]);
    expect([...callback.searchParams.keys()]).toEqual(["state", "uid"]);
    expect(callback.searchParams.get("uid")).toBe("42");
  });

  it("does not access sessions on an external identity provider", async () => {
    const { tick, fetch, assign } = setup("https://auth.happy-token.cn");
    await tick();
    expect(fetch).not.toHaveBeenCalled();
    expect(assign).not.toHaveBeenCalled();
  });

  it("waits for the SSO flow to reach the dashboard", async () => {
    const { tick, fetch } = setup(undefined, undefined, "/sso");
    await tick();
    expect(fetch).not.toHaveBeenCalled();
  });

  it.each(["", "0", "abc", "-1"])(
    "rejects invalid local user id %s",
    async (uid) => {
      const { tick, fetch } = setup(undefined, uid);
      await tick();
      expect(fetch).not.toHaveBeenCalled();
    },
  );

  it("rejects a mismatched or rejected server identity", async () => {
    for (const data of [
      { success: true, data: { id: 43 } },
      { success: false, data: { id: 42 } },
    ]) {
      const { tick, fetch, assign } = setup();
      fetch.mockResolvedValue({ ok: true, json: async () => data });
      await tick();
      expect(assign).not.toHaveBeenCalled();
    }
  });

  it("recovers after network failure without overlapping requests", async () => {
    const { tick, fetch, assign } = setup();
    fetch.mockRejectedValueOnce(new Error("offline"));
    await tick();
    expect(assign).not.toHaveBeenCalled();
    await tick();
    expect(assign).toHaveBeenCalledOnce();
  });
});
