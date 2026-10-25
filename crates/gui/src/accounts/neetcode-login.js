(async () => {
  if (window !== window.top || location.origin !== "https://neetcode.io" || window.__leetReadingSession) return;
  window.__leetReadingSession = true;
  let database;
  try {
    if (!indexedDB.databases) return;
    const databases = await indexedDB.databases();
    if (!databases.some(database => database.name === "firebaseLocalStorageDb")) return;
    database = await new Promise((resolve, reject) => {
      const request = indexedDB.open("firebaseLocalStorageDb");
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject();
    });
    if (!database.objectStoreNames.contains("firebaseLocalStorage")) return;
    const rows = await new Promise((resolve, reject) => {
      const request = database.transaction("firebaseLocalStorage", "readonly").objectStore("firebaseLocalStorage").getAll();
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject();
    });
    const user = rows.map(row => row.value).find(user => user?.apiKey === "AIzaSyD4emZpWF1MIsu6Z8O6yaMMcPxJ2Z38L8g" && user?.uid && user?.stsTokenManager?.refreshToken);
    if (user) window.ipc.postMessage(JSON.stringify({userAgent: navigator.userAgent, refreshToken: user.stsTokenManager.refreshToken, userId: user.uid}));
  } catch {
    // A signed-out page or a pending database transaction is retried by the host.
  } finally {
    database?.close();
    window.__leetReadingSession = false;
  }
})();
