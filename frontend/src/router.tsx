import {
  createRouter,
  Outlet,
  createRootRoute,
  createRoute,
  Navigate,
  redirect,
} from "@tanstack/solid-router";
import { Privacy } from "~/components/Privacy.tsx";

const rootRoute = createRootRoute({
  component: () => <Outlet />,
});

// const indexRoute = createRoute({
//   getParentRoute: () => rootRoute,
//   path: "/",
//   component: App,
// });

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  loader: () => {
    throw redirect({ to: "/privacy" });
  },
});

const privacyRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/privacy",
  component: Privacy,
});

const routeTree = rootRoute.addChildren([indexRoute, privacyRoute]);

export const router = createRouter({ routeTree });

declare module "@tanstack/solid-router" {
  interface Register {
    router: typeof router;
  }
}
