import { createRouter, Outlet, createRootRoute, createRoute } from "@tanstack/solid-router";
import { Privacy } from "~/components/Privacy.tsx";
import App from "~/App.tsx";
import IronMicHistory from "~/IronMicHistory.tsx";

const rootRoute = createRootRoute({
  component: () => <Outlet />,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: App,
});

// const indexRoute = createRoute({
//   getParentRoute: () => rootRoute,
//   path: "/",
//   loader: () => {
//     throw redirect({ to: "/privacy" });
//   },
// });

export const ironMicHistoryRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/ironmic/$year/$month",
  component: IronMicHistory,
});

const privacyRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/privacy",
  component: Privacy,
});

const routeTree = rootRoute.addChildren([indexRoute, ironMicHistoryRoute, privacyRoute]);

export const router = createRouter({ routeTree });

declare module "@tanstack/solid-router" {
  interface Register {
    router: typeof router;
  }
}
