import { AppSidebar } from "~/components/layout/AppSidebar";
import { SidebarProvider, SidebarTrigger } from "~/components/ui/Sidebar";
import { ParentProps } from "solid-js";

export default function Layout(props: ParentProps) {
  return (
    <SidebarProvider>
      <AppSidebar />
      <main>
        <SidebarTrigger />
        {props.children}
      </main>
    </SidebarProvider>
  );
}
