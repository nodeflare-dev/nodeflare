/**
 * Wave-pulse square loader: a row of rounded violet squares that brighten and scale up in
 * a staggered, looping wave. Used on the auth/authorization screens.
 */
export function SquareLoader({ className = '' }: { className?: string }) {
  return (
    <div className={`flex items-center justify-center gap-1.5 ${className}`} role="status" aria-label="Loading">
      {[0, 1, 2, 3, 4].map((i) => (
        <span
          key={i}
          className="h-2.5 w-2.5 rounded-[3px] bg-violet-600 animate-wave"
          style={{ animationDelay: `${i * 0.12}s` }}
        />
      ))}
    </div>
  );
}
