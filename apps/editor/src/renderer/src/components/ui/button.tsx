import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-md font-medium transition-[background-color,color,box-shadow,opacity] duration-150 disabled:pointer-events-none disabled:opacity-45 [&_svg]:pointer-events-none [&_svg]:size-3.5 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        primary:
          "bg-accent-gradient text-primary-foreground shadow-[0_1px_2px_rgb(0_0_0/0.25),inset_0_1px_0_rgb(255_255_255/0.18)] hover:opacity-92",
        secondary: "border bg-secondary text-secondary-foreground hover:bg-accent",
        ghost: "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
        destructive: "text-destructive hover:bg-destructive/12",
      },
      size: {
        sm: "h-6 px-2 text-xs",
        md: "h-7 px-2.5 text-xs",
        lg: "h-8 px-3.5 text-sm",
        icon: "size-6",
        "icon-md": "size-7",
      },
    },
    defaultVariants: { variant: "secondary", size: "md" },
  },
);

export function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: ComponentProps<"button"> & VariantProps<typeof buttonVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "button";
  return <Comp data-slot="button" className={cn(buttonVariants({ variant, size, className }))} {...props} />;
}
