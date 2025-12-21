import { createMediaQuery } from "@solid-primitives/media";
import { Show } from "solid-js";
import {
  NavigationMenu,
  NavigationMenuContent,
  NavigationMenuDescription,
  NavigationMenuIcon,
  NavigationMenuItem,
  NavigationMenuLabel,
  NavigationMenuLink,
  NavigationMenuTrigger,
} from "~/components/ui/NavigationMenu";
import { Link } from "@tanstack/solid-router";

export const TopNavbar = () => {
  const isSmall = createMediaQuery("(max-width: 767px)");

  return (
    <header class="fixed inset-x-0 top-0 z-50 w-full border-b bg-secondary px-4 backdrop-blur **:no-underline md:px-6">
      <div class="container mx-auto flex h-16 max-w-screen-2xl items-center justify-between gap-4">
        <Show when={isSmall()}>
          <MobileNav />
        </Show>
        <div class="flex items-center gap-6 max-md:justify-between">
          <LogoButton />
        </div>
        <Show when={!isSmall()}>
          <FullNav />
        </Show>
      </div>
    </header>
  );
};

const MobileNav = () => {
  return <div></div>;
};

const LogoButton = () => {
  return (
    <Link to="/">
      <div class="flex cursor-pointer items-center space-x-2 text-primary transition-colors hover:text-primary/90">
        <img class="w-48" src="/images/logo.png" alt="logo" />
      </div>
    </Link>
  );
};

const FullNav = () => {
  return (
    <NavigationMenu>
      {/*<NavigationMenuItem>*/}
      {/*  <NavigationMenuTrigger>*/}
      {/*    <Link to="/privacy">Privacy</Link>*/}
      {/*  </NavigationMenuTrigger>*/}
      {/*</NavigationMenuItem>*/}
      {/*<NavigationMenuTrigger as="a" href="https://github.com/kobaltedev/kobalte" target="_blank">*/}
      {/*  GitHub*/}
      {/*</NavigationMenuTrigger>*/}
      {/*<NavigationMenuTrigger as="a" href="https://github.com/kobaltedev/kobalte" target="_blank">*/}
      {/*  GitHub*/}
      {/*</NavigationMenuTrigger>*/}
      {/*<NavigationMenuTrigger as="a" href="https://github.com/kobaltedev/kobalte" target="_blank">*/}
      {/*  GitHub*/}
      {/*</NavigationMenuTrigger>*/}
      {/*<NavigationMenuTrigger as="a" href="https://github.com/kobaltedev/kobalte" target="_blank">*/}
      {/*  GitHub*/}
      {/*</NavigationMenuTrigger>*/}
    </NavigationMenu>
  );
};

// ("use client");
// import * as React from "react";
// import { useEffect, useState, useRef, useId } from "react";
// import { SearchIcon } from "lucide-react";
// import { Button } from "@/components/ui/button";
// import { Input } from "@/components/ui/input";
// import {
//   NavigationMenu,
//   NavigationMenuItem,
//   NavigationMenuLink,
//   NavigationMenuList,
// } from "@/components/ui/navigation-menu";
// import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
// import { cn } from "@/lib/utils";
// import type { ComponentProps } from "react";
// // Simple logo component for the navbar
// const Logo = (props: React.SVGAttributes<SVGElement>) => {
//   return (
//     <svg
//       width="1em"
//       height="1em"
//       viewBox="0 0 324 323"
//       fill="currentColor"
//       xmlns="http://www.w3.org/2000/svg"
//       {...(props as any)}
//     >
//       <rect
//         x="88.1023"
//         y="144.792"
//         width="151.802"
//         height="36.5788"
//         rx="18.2894"
//         transform="rotate(-38.5799 88.1023 144.792)"
//         fill="currentColor"
//       />
//       <rect
//         x="85.3459"
//         y="244.537"
//         width="151.802"
//         height="36.5788"
//         rx="18.2894"
//         transform="rotate(-38.5799 85.3459 244.537)"
//         fill="currentColor"
//       />
//     </svg>
//   );
// };
// // Hamburger icon component
// const HamburgerIcon = ({ className, ...props }: React.SVGAttributes<SVGElement>) => (
//   <svg
//     className={cn("pointer-events-none", className)}
//     width={16}
//     height={16}
//     viewBox="0 0 24 24"
//     fill="none"
//     stroke="currentColor"
//     strokeWidth="2"
//     strokeLinecap="round"
//     strokeLinejoin="round"
//     xmlns="http://www.w3.org/2000/svg"
//     {...(props as any)}
//   >
//     <path
//       d="M4 12L20 12"
//       className="origin-center -translate-y-[7px] transition-all duration-300 ease-[cubic-bezier(.5,.85,.25,1.1)] group-aria-expanded:translate-x-0 group-aria-expanded:translate-y-0 group-aria-expanded:rotate-[315deg]"
//     />
//     <path
//       d="M4 12H20"
//       className="origin-center transition-all duration-300 ease-[cubic-bezier(.5,.85,.25,1.8)] group-aria-expanded:rotate-45"
//     />
//     <path
//       d="M4 12H20"
//       className="origin-center translate-y-[7px] transition-all duration-300 ease-[cubic-bezier(.5,.85,.25,1.1)] group-aria-expanded:translate-y-0 group-aria-expanded:rotate-[135deg]"
//     />
//   </svg>
// );
// // Types
// export interface Navbar04NavItem {
//   href?: string;
//   label: string;
// }
// export interface Navbar04Props extends React.HTMLAttributes<HTMLElement> {
//   logo?: React.ReactNode;
//   logoHref?: string;
//   navigationLinks?: Navbar04NavItem[];
//   signInText?: string;
//   signInHref?: string;
//   cartText?: string;
//   cartHref?: string;
//   cartCount?: number;
//   searchPlaceholder?: string;
//   onSignInClick?: () => void;
//   onCartClick?: () => void;
//   onSearchSubmit?: (query: string) => void;
// }
// // Default navigation links
// const defaultNavigationLinks: Navbar04NavItem[] = [
//   { href: "#", label: "Products" },
//   { href: "#", label: "Categories" },
//   { href: "#", label: "Deals" },
// ];
// export const Navbar04 = React.forwardRef<HTMLElement, Navbar04Props>(
//   (
//     {
//       className,
//       logo = <Logo />,
//       logoHref = "#",
//       navigationLinks = defaultNavigationLinks,
//       signInText = "Sign In",
//       signInHref = "#signin",
//       cartText = "Cart",
//       cartHref = "#cart",
//       cartCount = 2,
//       searchPlaceholder = "Search...",
//       onSignInClick,
//       onCartClick,
//       onSearchSubmit,
//       ...props
//     },
//     ref,
//   ) => {
//     const [isMobile, setIsMobile] = useState(false);
//     const containerRef = useRef<HTMLElement>(null);
//     const searchId = useId();
//     useEffect(() => {
//       const checkWidth = () => {
//         if (containerRef.current) {
//           const width = containerRef.current.offsetWidth;
//           setIsMobile(width < 768); // 768px is md breakpoint
//         }
//       };
//       checkWidth();
//       const resizeObserver = new ResizeObserver(checkWidth);
//       if (containerRef.current) {
//         resizeObserver.observe(containerRef.current);
//       }
//       return () => {
//         resizeObserver.disconnect();
//       };
//     }, []);
//     // Combine refs
//     const combinedRef = React.useCallback(
//       (node: HTMLElement | null) => {
//         containerRef.current = node;
//         if (typeof ref === "function") {
//           ref(node);
//         } else if (ref) {
//           ref.current = node;
//         }
//       },
//       [ref],
//     );
//     const handleSearchSubmit = (e: React.FormEvent<HTMLFormElement>) => {
//       e.preventDefault();
//       const formData = new FormData(e.currentTarget);
//       const query = formData.get("search") as string;
//       if (onSearchSubmit) {
//         onSearchSubmit(query);
//       }
//     };
//     return (
//       <header
//         ref={combinedRef}
//         className={cn(
//           "sticky top-0 z-50 w-full border-b bg-background/95 px-4 backdrop-blur supports-[backdrop-filter]:bg-background/60 md:px-6 [&_*]:no-underline",
//           className,
//         )}
//         {...(props as any)}
//       >
//         <div className="container mx-auto flex h-16 max-w-screen-2xl items-center justify-between gap-4">
//           {/* Left side */}
//           <div className="flex flex-1 items-center gap-2">
//             {/* Mobile menu trigger */}
//             {isMobile && (
//               <Popover>
//                 <PopoverTrigger asChild>
//                   <Button
//                     className="group h-9 w-9 hover:bg-accent hover:text-accent-foreground"
//                     variant="ghost"
//                     size="icon"
//                   >
//                     <HamburgerIcon />
//                   </Button>
//                 </PopoverTrigger>
//                 <PopoverContent align="start" className="w-64 p-1">
//                   <NavigationMenu className="max-w-none">
//                     <NavigationMenuList className="flex-col items-start gap-0">
//                       {navigationLinks.map((link, index) => (
//                         <NavigationMenuItem key={index} className="w-full">
//                           <button
//                             onClick={(e) => e.preventDefault()}
//                             className="flex w-full cursor-pointer items-center rounded-md px-3 py-2 text-sm font-medium no-underline transition-colors hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground"
//                           >
//                             {link.label}
//                           </button>
//                         </NavigationMenuItem>
//                       ))}
//                       <NavigationMenuItem className="w-full" role="presentation" aria-hidden={true}>
//                         <div
//                           role="separator"
//                           aria-orientation="horizontal"
//                           className="-mx-1 my-1 h-px bg-border"
//                         />
//                       </NavigationMenuItem>
//                       <NavigationMenuItem className="w-full">
//                         <button
//                           onClick={(e) => {
//                             e.preventDefault();
//                             if (onSignInClick) onSignInClick();
//                           }}
//                           className="flex w-full cursor-pointer items-center rounded-md px-3 py-2 text-sm font-medium no-underline transition-colors hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground"
//                         >
//                           {signInText}
//                         </button>
//                       </NavigationMenuItem>
//                       <NavigationMenuItem className="w-full">
//                         <Button
//                           size="sm"
//                           className="mt-0.5 w-full text-left text-sm"
//                           onClick={(e) => {
//                             e.preventDefault();
//                             if (onCartClick) onCartClick();
//                           }}
//                         >
//                           <span className="flex items-baseline gap-2">
//                             {cartText}
//                             <span className="text-xs text-primary-foreground/60">{cartCount}</span>
//                           </span>
//                         </Button>
//                       </NavigationMenuItem>
//                     </NavigationMenuList>
//                   </NavigationMenu>
//                 </PopoverContent>
//               </Popover>
//             )}
//             {/* Main nav */}
//             <div className="flex flex-1 items-center gap-6 max-md:justify-between">
//               <button
//                 onClick={(e) => e.preventDefault()}
//                 className="flex cursor-pointer items-center space-x-2 text-primary transition-colors hover:text-primary/90"
//               >
//                 <div className="text-2xl">{logo}</div>
//                 <span className="hidden text-xl font-bold sm:inline-block">shadcn.io</span>
//               </button>
//               {/* Navigation menu */}
//               {!isMobile && (
//                 <NavigationMenu className="flex">
//                   <NavigationMenuList className="gap-1">
//                     {navigationLinks.map((link, index) => (
//                       <NavigationMenuItem key={index}>
//                         <NavigationMenuLink
//                           href={link.href}
//                           onClick={(e) => e.preventDefault()}
//                           className="group inline-flex h-10 w-max cursor-pointer items-center justify-center rounded-md bg-background px-4 py-1.5 py-2 text-sm font-medium text-muted-foreground transition-colors hover:text-primary focus:bg-accent focus:text-accent-foreground focus:outline-none disabled:pointer-events-none disabled:opacity-50"
//                         >
//                           {link.label}
//                         </NavigationMenuLink>
//                       </NavigationMenuItem>
//                     ))}
//                   </NavigationMenuList>
//                 </NavigationMenu>
//               )}
//               {/* Search form */}
//               <form onSubmit={handleSearchSubmit} className="relative">
//                 <Input
//                   id={searchId}
//                   name="search"
//                   className="peer h-8 ps-8 pe-2"
//                   placeholder={searchPlaceholder}
//                   type="search"
//                 />
//                 <div className="pointer-events-none absolute inset-y-0 start-0 flex items-center justify-center ps-2 text-muted-foreground/80 peer-disabled:opacity-50">
//                   <SearchIcon size={16} />
//                 </div>
//               </form>
//             </div>
//           </div>
//           {/* Right side */}
//           {!isMobile && (
//             <div className="flex items-center gap-3">
//               <Button
//                 variant="ghost"
//                 size="sm"
//                 className="text-sm font-medium hover:bg-accent hover:text-accent-foreground"
//                 onClick={(e) => {
//                   e.preventDefault();
//                   if (onSignInClick) onSignInClick();
//                 }}
//               >
//                 {signInText}
//               </Button>
//               <Button
//                 size="sm"
//                 className="h-9 rounded-md px-4 text-sm font-medium shadow-sm"
//                 onClick={(e) => {
//                   e.preventDefault();
//                   if (onCartClick) onCartClick();
//                 }}
//               >
//                 <span className="flex items-baseline gap-2">
//                   {cartText}
//                   <span className="text-xs text-primary-foreground/60">{cartCount}</span>
//                 </span>
//               </Button>
//             </div>
//           )}
//         </div>
//       </header>
//     );
//   },
// );
// Navbar04.displayName = "Navbar04";
// export { Logo, HamburgerIcon };
