import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router-dom";
import { initializeAuthFromEnv, queryClient } from "@/api";
import { router } from "./router";
import { Toaster } from "@/components/ui/sonner";
import { ErrorBoundary } from "@/components/ErrorBoundary";

// Build-time env から API Key / JWT を読み込み、デフォルト認証をセット
initializeAuthFromEnv();

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ErrorBoundary>
        <RouterProvider router={router} />
        <Toaster position="top-right" richColors />
      </ErrorBoundary>
    </QueryClientProvider>
  );
}

export default App;
