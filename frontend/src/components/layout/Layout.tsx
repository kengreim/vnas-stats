// import { AppSidebar } from "~/components/layout/AppSidebar";
// import { SidebarProvider, SidebarTrigger } from "~/components/ui/Sidebar";
import { ParentProps } from "solid-js";
import { TopNavbar } from "~/components/layout/TopNavbar.tsx";

export function Layout(props: ParentProps) {
  return (
    // <SidebarProvider>
    //   <AppSidebar />
    //   <main>
    //     <SidebarTrigger />
    //     {props.children}
    //   </main>
    // </SidebarProvider>
    <main>
      <TopNavbar />
      {props.children}
    </main>
  );
}
