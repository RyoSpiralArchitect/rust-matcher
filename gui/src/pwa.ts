const registerServiceWorker = () => {
  if (!("serviceWorker" in navigator)) {
    return;
  }

  window.addEventListener("load", () => {
    navigator.serviceWorker
      .register("/sw.js")
      .catch((error) => {
        console.warn("serviceWorker registration failed", error);
      });
  });
};

registerServiceWorker();
