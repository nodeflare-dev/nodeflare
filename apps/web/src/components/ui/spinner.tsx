import { cn } from '@/lib/utils';

interface SpinnerProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

export function Spinner({ size = 'md', className }: SpinnerProps) {
  const sizeClasses = {
    sm: 'w-5 h-5 border-[3px]',
    md: 'w-8 h-8 border-4',
    lg: 'w-10 h-10 border-4',
  };

  return (
    <div
      className={cn(
        'rounded-full border-gray-200 border-t-violet-600 animate-spin',
        sizeClasses[size],
        className
      )}
    />
  );
}

export function PageSpinner() {
  return (
    <div className="flex items-center justify-center py-16">
      <Spinner size="md" />
    </div>
  );
}
