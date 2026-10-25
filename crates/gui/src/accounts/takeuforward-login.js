(async () => {
  if (window !== window.top || location.origin !== "https://takeuforward.org" || location.pathname === "/get-started" || window.__leetCheckingAccount) return;
  window.__leetCheckingAccount = true;
  try {
    const response = await fetch("https://backend-go.takeuforward.org/api/v1/auth/me", {credentials: "include"});
    if (!response.ok) return;
    const account = await response.json();
    if (account.success && account.data?.logged_in && !account.data.is_banned) {
      window.ipc.postMessage(JSON.stringify({userAgent: navigator.userAgent}));
    }
  } catch {
    // The host retries while the login window remains open.
  } finally {
    window.__leetCheckingAccount = false;
  }
})();
