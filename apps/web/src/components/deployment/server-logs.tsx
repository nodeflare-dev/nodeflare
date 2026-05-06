'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { useServerLogsWebSocket, ServerLogLine } from '@/hooks/use-websocket';
import { cn } from '@/lib/utils';

// Constants
const MAX_LOGS_IN_MEMORY = 1000;

interface ServerLogsProps {
  workspaceId: string;
  serverId: string;
  className?: string;
}

const logLevelColors: Record<ServerLogLine['level'], string> = {
  debug: 'text-gray-400',
  info: 'text-blue-400',
  warn: 'text-yellow-400',
  error: 'text-red-400',
};

const logLevelLabels: Record<ServerLogLine['level'], string> = {
  debug: 'DEBUG',
  info: 'INFO',
  warn: 'WARN',
  error: 'ERROR',
};

export function ServerLogs({ workspaceId, serverId, className }: ServerLogsProps) {
  const [logs, setLogs] = useState<ServerLogLine[]>([]);
  const [filter, setFilter] = useState<ServerLogLine['level'] | 'all'>('all');
  const containerRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  const { isConnected } = useServerLogsWebSocket(workspaceId, serverId, {
    onLog: (log) => {
      setLogs((prev) => {
        // Keep last N logs to prevent memory issues
        const newLogs = [...prev, log];
        return newLogs.slice(-MAX_LOGS_IN_MEMORY);
      });
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

  const filteredLogs = useMemo(() => {
    if (filter === 'all') return logs;
    return logs.filter((log) => log.level === filter);
  }, [logs, filter]);

  return (
    <div className={cn('flex flex-col', className)}>
      {/* Header */}
      <div className="flex items-center justify-between border-b bg-muted/50 px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">Server Logs</span>
          <span
            className={cn(
              'h-2 w-2 rounded-full',
              isConnected ? 'bg-green-500 animate-pulse' : 'bg-gray-500'
            )}
          />
          <span className="text-xs text-muted-foreground">
            {isConnected ? 'Live' : 'Connecting...'}
          </span>
        </div>

        {/* Filter */}
        <div className="flex items-center gap-2">
          <select
            value={filter}
            onChange={(e) => setFilter(e.target.value as typeof filter)}
            className="rounded border bg-background px-2 py-1 text-xs"
          >
            <option value="all">All</option>
            <option value="debug">Debug</option>
            <option value="info">Info</option>
            <option value="warn">Warn</option>
            <option value="error">Error</option>
          </select>
          <button
            onClick={() => setLogs([])}
            className="text-xs text-muted-foreground hover:text-foreground"
          >
            Clear
          </button>
        </div>
      </div>

      {/* Logs container */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="relative flex-1 overflow-auto bg-black p-4 font-mono text-xs"
        style={{ minHeight: '300px', maxHeight: '500px' }}
      >
        {filteredLogs.length === 0 ? (
          <div className="text-gray-500">
            {logs.length === 0 ? 'Waiting for logs...' : 'No logs match the current filter'}
          </div>
        ) : (
          filteredLogs.map((log, index) => (
            <div key={`${log.timestamp}-${index}`} className="flex whitespace-pre-wrap">
              <span className="text-gray-500 select-none w-20">
                {new Date(log.timestamp).toLocaleTimeString()}
              </span>
              <span className={cn('w-12 select-none', logLevelColors[log.level])}>
                [{logLevelLabels[log.level]}]
              </span>
              <span className="text-gray-200 flex-1">{log.line}</span>
            </div>
          ))
        )}
      </div>

      {/* Auto-scroll button */}
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
