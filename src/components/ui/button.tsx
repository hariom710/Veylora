import * as React from 'react';
import { cn } from './utils';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'destructive' | 'outline' | 'secondary' | 'ghost' | 'link';
  size?: 'default' | 'sm' | 'lg' | 'icon';
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'default', size = 'default', asChild = false, ...props }, ref) => {
    return (
      <button
        ref={ref}
        className={cn(
          'inline-flex items-center justify-center rounded-button transition-colors disabled:pointer-events-none disabled:opacity-50',
          variant === 'default' &&
            'bg-accent text-foreground text-[17px] font-semibold hover:bg-accent-hover',
          variant === 'destructive' &&
            'bg-danger text-foreground text-[17px] font-semibold hover:opacity-90',
          variant === 'outline' &&
            'border border-border bg-transparent text-foreground text-sm font-medium hover:border-accent hover:text-accent',
          variant === 'secondary' &&
            'bg-surface border border-border text-foreground text-sm font-medium hover:bg-elevated',
          variant === 'ghost' && 'text-muted text-sm font-medium hover:bg-white/5 hover:text-foreground',
          variant === 'link' && 'text-accent text-sm font-medium underline-offset-4 hover:underline',
          size === 'default' && 'h-11 px-6',
          size === 'sm' && 'h-9 px-3 text-sm',
          size === 'lg' && 'h-11 px-8',
          size === 'icon' && 'h-10 w-10',
          className,
        )}
        {...props}
      />
    );
  },
);
Button.displayName = 'Button';

export { Button };
