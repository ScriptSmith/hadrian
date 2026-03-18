import { Info } from "lucide-react";
import { Card, CardContent } from "@/components/Card/Card";

interface PageNoticeProps {
  title: string;
  message: string;
}

export function PageNotice({ title, message }: PageNoticeProps) {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <Card className="max-w-lg">
        <CardContent className="flex flex-col items-center gap-4 p-8 text-center">
          <div className="flex h-12 w-12 items-center justify-center rounded-full bg-muted">
            <Info className="h-6 w-6 text-muted-foreground" />
          </div>
          <h2 className="text-xl font-semibold">{title}</h2>
          <p className="text-muted-foreground">{message}</p>
        </CardContent>
      </Card>
    </div>
  );
}
