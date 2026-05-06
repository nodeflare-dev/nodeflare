'use client';

import { useEffect, useRef, useState } from 'react';
import { useBuildLogsWebSocket, BuildLogLine } from '@/hooks/use-websocket';
import { cn } from '@/lib/utils';

interface BuildLogsProps {
  deploymentId: string;
  initialLogs?: string;
  className?: string;
}

export function BuildLogs({ deploymentId, initialLogs, className }: BuildLogsProps) {
  const [logs, setLogs] = useState<BuildLogLine[]>([]);
  const containerRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  // Parse initial logs
  useEffect(() => {
    if (initialLogs) {
      const lines = initialLogs.split('\n').map((line) => ({
        deployment_id: deploymentId,
        line,
        stream: 'stdout' as const,
        timestamp: new Date().toISOString(),
      }));
      setLogs(lines);
    }
  }, [deploymentId, initialLogs]);

  const { isConnected } = useBuildLogsWebSocket(deploymentId, {
    onLog: (log) => {
      setLogs((prev) => [...prev, log]);
    },
  });

  // Auto-scroll to bottom
  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [logs, autoScroll]);

  // Detect if user scrolled up
  const handleScroll = () => {
    if (containerRef.current) {
      const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
      const isAtBottom = scrollTop + clientHeight >= scrollHeight - 10;
      setAutoScroll(isAtBottom);
    }
  };

  return (
    <div className={cn('flex flex-col', className)}>
      {/* Header */}
      <div className="flex items-center justify-between border-b bg-muted/50 px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">Build Logs</span>
          <span
            className={cn(
              'h-2 w-2 rounded-full',
              isConnected ? 'bg-green-500' : 'bg-gray-500'
            )}
          />
        </div>
        <span className="text-xs text-muted-foreground">
          {logs.length} lines
        </span>
      </div>

      {/* Logs container */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-auto bg-black p-4 font-mono text-xs"
        style={{ minHeight: '300px', maxHeight: '500px' }}
      >
        {logs.length === 0 ? (
          <div className="text-gray-500">Waiting for logs...</div>
        ) : (
          logs.map((log, index) => (
            <div
              key={index}
              className={cn(
                'whitespace-pre-wrap',
                log.stream === 'stderr' ? 'text-red-400' : 'text-green-400'
              )}
            >
              <span className="text-gray-500 select-none">
                {new Date(log.timestamp).toLocaleTimeString()}
              </span>{' '}
              {log.line}
            </div>
          ))
        )}
      </div>

      {/* Auto-scroll indicator */}
      {!autoScroll && (
        <button
          onClick={() => {
            setAutoScroll(true);
            if (containerRef.current) {
              containerRef.current.scrollTop = containerRef.current.scrollHeight;
            }
          }}
          className="absolute bottom-4 right-4 rounded-full bg-primary px-3 py-1 text-xs text-primary-foreground shadow-lg"
        >
          Scroll to bottom
        </button>
      )}
    </div>
  );
}
