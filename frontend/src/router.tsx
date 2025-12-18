import { createRouter, Outlet, createRootRoute, createRoute } from "@tanstack/solid-router";
import App from "./App";
import { Privacy } from "~/components/Privacy.tsx";

const rootRoute = createRootRoute({
  component: () => <Outlet />,
});

// const indexRoute = createRoute({
//   getParentRoute: () => rootRoute,
//   path: "/",
//   component: App,
// });

const privacyRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/privacy",
  component: Privacy,
});

const routeTree = rootRoute.addChildren([privacyRoute]);

export const router = createRouter({ routeTree });

declare module "@tanstack/solid-router" {
  interface Register {
    router: typeof router;
  }
}
