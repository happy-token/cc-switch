(() => {
  if (window.location.origin !== "https://gateway.happy-token.cn") return;
  let running = false;
  let complete = false;
  const timer = setInterval(async () => {
    // Wait for the existing SSO page to finish establishing its Gateway session.
    if (
      running ||
      complete ||
      ![
        "/dashboard",
        "/keys",
        "/profile",
        "/console",
        "/console/token",
        "/console/personal",
      ].includes(location.pathname)
    )
      return;
    const uid = localStorage.getItem("uid");
    if (!uid || !/^[1-9]\d{0,18}$/.test(uid)) return;
    running = true;
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 10000);
    try {
      const response = await fetch("/api/user/self", {
        credentials: "same-origin",
        headers: { "New-Api-User": uid, Accept: "application/json" },
        signal: controller.signal,
      });
      const result = await response.json();
      if (
        !response.ok ||
        result.success !== true ||
        String(result.data?.id) !== uid
      )
        return;
      complete = true;
      clearInterval(timer);
      location.assign(
        "https://gateway.happy-token.cn/__happy_switch_authenticated?state=__HAPPY_NONCE__&uid=" +
          encodeURIComponent(uid),
      );
    } catch {
      // A transient network failure must not prevent the user completing login.
    } finally {
      clearTimeout(timeout);
      running = false;
    }
  }, 1000);
})();
