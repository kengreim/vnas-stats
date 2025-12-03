import { createRouter, Route, RootRoute, Outlet } from "@tanstack/solid-router";
import App from "./App";

const rootRoute = new RootRoute({
  component: () => <div><Outlet /></div>,
});

const indexRoute = new Route({
  getParentRoute: () => rootRoute,
  path: "/",
  component: App,
});

const routeTree = rootRoute.addChildren([indexRoute]);

export const router = createRouter({ routeTree });

declare module "@tanstack/solid-router" {
  interface Register {
    router: typeof router;
  }
}
