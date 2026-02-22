import type { ReactNode } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/Card/Card";
import { Skeleton } from "@/components/Skeleton/Skeleton";

export interface StatCardProps {
  title: string;
  icon?: ReactNode;
  isLoading?: boolean;
  children: ReactNode;
}

export function StatCard({ title, icon, isLoading, children }: StatCardProps) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="text-sm font-medium text-muted-foreground">{title}</CardTitle>
        {icon && <span className="text-muted-foreground">{icon}</span>}
      </CardHeader>
      <CardContent>{isLoading ? <Skeleton className="h-8 w-24" /> : children}</CardContent>
    </Card>
  );
}

export interface StatValueProps {
  value: string | number;
  className?: string;
}

export function StatValue({ value, className = "" }: StatValueProps) {
  return <div className={`text-2xl font-bold ${className}`}>{value}</div>;
}
