import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, LogOut, Monitor, ShieldOff } from "lucide-react";
import type { SessionInfo, SessionListResponse } from "@/api/generated";
import {
  userSessionsDeleteAllMutation,
  userSessionsDeleteOneMutation,
  userSessionsListQueryKey,
} from "@/api/generated/@tanstack/react-query.gen";
import { SessionCard } from "./SessionCard";

export interface SessionsPanelProps {
  userId: string;
  sessions: SessionListResponse;
  className?: string;
}

export function SessionsPanel({ userId, sessions, className = "" }: SessionsPanelProps) {
  const queryClient = useQueryClient();
  const [showConfirmAll, setShowConfirmAll] = useState(false);
  const [revokingSessionId, setRevokingSessionId] = useState<string | null>(null);

  const invalidateSessions = () => {
    queryClient.invalidateQueries({
      queryKey: userSessionsListQueryKey({ path: { user_id: userId } }),
    });
  };

  const deleteAllMutation = useMutation({
    ...userSessionsDeleteAllMutation(),
    onSuccess: () => {
      setShowConfirmAll(false);
      invalidateSessions();
    },
  });

  const deleteOneMutation = useMutation({
    ...userSessionsDeleteOneMutation(),
    onSuccess: () => {
      setRevokingSessionId(null);
      invalidateSessions();
    },
    onError: () => {
      setRevokingSessionId(null);
    },
  });

  const handleRevokeOne = (sessionId: string) => {
    setRevokingSessionId(sessionId);
    deleteOneMutation.mutate({
      path: { user_id: userId, session_id: sessionId },
    });
  };

  const handleRevokeAll = () => {
    deleteAllMutation.mutate({
      path: { user_id: userId },
    });
  };

  // If enhanced sessions not enabled, show message
  if (!sessions.enhanced_enabled) {
    return (
      <div
        className={`rounded-lg border border-yellow-200 dark:border-yellow-800 bg-yellow-50 dark:bg-yellow-900/20 p-6 ${className}`}
      >
        <div className="flex items-start gap-3">
          <ShieldOff className="w-5 h-5 text-yellow-600 dark:text-yellow-400 flex-shrink-0 mt-0.5" />
          <div>
            <h3 className="text-sm font-medium text-yellow-800 dark:text-yellow-200">
              Enhanced Session Management Not Enabled
            </h3>
            <p className="mt-1 text-sm text-yellow-700 dark:text-yellow-300">
              Session tracking is not enabled for this deployment. Enable{" "}
              <code className="text-xs bg-yellow-100 dark:bg-yellow-800 px-1 py-0.5 rounded">
                auth.ui.session.enhanced.enabled = true
              </code>{" "}
              in your configuration to track and manage user sessions.
            </p>
          </div>
        </div>
      </div>
    );
  }

  // Empty state
  if (sessions.data.length === 0) {
    return (
      <div className={`text-center py-12 ${className}`}>
        <Monitor className="w-12 h-12 text-gray-300 dark:text-gray-600 mx-auto mb-3" />
        <h3 className="text-sm font-medium text-gray-900 dark:text-gray-100">No Active Sessions</h3>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
          This user has no active browser sessions.
        </p>
      </div>
    );
  }

  return (
    <div className={className}>
      {/* Header with Force Logout All button */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {sessions.data.length} active session{sessions.data.length !== 1 ? "s" : ""}
          </p>
        </div>

        {!showConfirmAll ? (
          <button
            onClick={() => setShowConfirmAll(true)}
            className="inline-flex items-center gap-2 px-3 py-2 text-sm font-medium text-red-700 hover:text-red-800 dark:text-red-400 dark:hover:text-red-300 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-lg transition-colors"
          >
            <LogOut className="w-4 h-4" />
            Force Logout All
          </button>
        ) : (
          <div className="flex items-center gap-2">
            <span className="text-sm text-gray-500 dark:text-gray-400">Revoke all sessions?</span>
            <button
              onClick={() => setShowConfirmAll(false)}
              disabled={deleteAllMutation.isPending}
              className="px-3 py-1.5 text-sm font-medium text-gray-600 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-md transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleRevokeAll}
              disabled={deleteAllMutation.isPending}
              className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium text-white bg-red-600 hover:bg-red-700 disabled:opacity-50 disabled:cursor-not-allowed rounded-md transition-colors"
            >
              {deleteAllMutation.isPending ? (
                <>
                  <span className="animate-spin">...</span>
                  Revoking...
                </>
              ) : (
                <>
                  <AlertTriangle className="w-4 h-4" />
                  Confirm
                </>
              )}
            </button>
          </div>
        )}
      </div>

      {/* Session list */}
      <div className="space-y-3">
        {sessions.data.map((session: SessionInfo) => (
          <SessionCard
            key={session.id}
            session={session}
            onRevoke={handleRevokeOne}
            isRevoking={revokingSessionId === session.id}
          />
        ))}
      </div>
    </div>
  );
}
