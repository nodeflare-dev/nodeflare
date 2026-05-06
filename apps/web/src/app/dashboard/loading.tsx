export default function DashboardLoading() {
  return (
    <div className="space-y-6">
      {/* Header skeleton */}
      <div className="flex items-center justify-between">
        <div className="h-8 w-40 bg-gray-200 rounded animate-pulse" />
        <div className="h-8 w-24 bg-gray-100 rounded animate-pulse" />
      </div>

      {/* Stats cards skeleton */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {[...Array(3)].map((_, i) => (
          <div key={i} className="h-24 bg-gray-100 rounded-lg animate-pulse" />
        ))}
      </div>

      {/* Content skeleton */}
      <div className="h-64 bg-gray-100 rounded-lg animate-pulse" />
    </div>
  );
}
