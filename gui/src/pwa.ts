const registerServiceWorker = () => {
  const isLocalhost =
    location.hostname === "localhost" || location.hostname === "127.0.0.1";
  const isSecureContext = location.protocol === "https:" || isLocalhost;
  const isSupported = "serviceWorker" in navigator;

  if (!isSupported || !isSecureContext || import.meta.env.DEV) {
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
