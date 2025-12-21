// import { AppSidebar } from "~/components/layout/AppSidebar";
// import { SidebarProvider, SidebarTrigger } from "~/components/ui/Sidebar";
import { ParentProps } from "solid-js";
import { Footer } from "~/components/layout/Footer.tsx";
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
    <>
      <TopNavbar />
      <div class="min-h-dvh pt-16.25">
        <main>{props.children}</main>
        <Footer />
      </div>
    </>
  );
}
