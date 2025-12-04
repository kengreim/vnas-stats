import { render } from "solid-js/web";
import { RouterProvider } from "@tanstack/solid-router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { router } from "./router";
import "@/app.css";

const queryClient = new QueryClient();

render(
  () => (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  ),
  document.getElementById("root") as HTMLElement,
);
