(async () => {
  if (location.hostname !== "neetcode.io") throw new Error("Run this on neetcode.io after signing in.");
  const database = await new Promise((resolve, reject) => {
    const request = indexedDB.open("firebaseLocalStorageDb");
    request.onupgradeneeded = () => { request.transaction.abort(); reject(new Error("Sign in to NeetCode first.")); };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(new Error("Could not read the sign-in session."));
  });
  try {
    if (!database.objectStoreNames.contains("firebaseLocalStorage")) throw new Error("Sign in to NeetCode first.");
    const rows = await new Promise((resolve, reject) => {
      const request = database.transaction("firebaseLocalStorage", "readonly").objectStore("firebaseLocalStorage").getAll();
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(new Error("Could not read the sign-in session."));
    });
    const user = rows.map(row => row.value).find(user => user?.apiKey === "AIzaSyD4emZpWF1MIsu6Z8O6yaMMcPxJ2Z38L8g" && user?.uid && user?.stsTokenManager?.refreshToken);
    if (!user) throw new Error("Sign in to NeetCode first.");
    copy(JSON.stringify({refreshToken: user.stsTokenManager.refreshToken, userID: user.uid}));
    console.info("Session copied. Paste it into verd's hidden NeetCode sign-in field.");
  } finally { database.close(); }
})();
