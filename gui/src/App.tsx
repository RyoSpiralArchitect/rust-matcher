import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router-dom";
import { queryClient, setApiKey } from "@/api";
import { router } from "./router";
import { Toaster } from "@/components/ui/sonner";

// Initialize API Key from environment (Phase 1)
const apiKey = import.meta.env.VITE_API_KEY;
if (apiKey) {
  setApiKey(apiKey);
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <Toaster position="top-right" richColors />
    </QueryClientProvider>
  );
}

export default App;
