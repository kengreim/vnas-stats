import { ComponentProps, ParentComponent, splitProps } from "solid-js";
import { cn } from "~/libs/cn.ts";

export const Prose: ParentComponent<ComponentProps<"div">> = (props) => {
  const [local, children, others] = splitProps(props, ["class"], ["children"]);
  return (
    <div class={cn("prose", local.class)} {...others}>
      {children.children}
    </div>
  );
};
