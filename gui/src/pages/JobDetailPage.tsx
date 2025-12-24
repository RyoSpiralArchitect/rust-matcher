import { useParams, Link } from "react-router-dom";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function JobDetailPage() {
  const { jobId } = useParams<{ jobId: string }>();

  // TODO: useQuery で job詳細を取得

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Link to="/jobs" className="hover:underline">
          Jobs
        </Link>
        <span>/</span>
        <span>{jobId}</span>
      </div>

      <h1 className="text-2xl font-bold">Job Detail: {jobId}</h1>

      <Card>
        <CardHeader>
          <CardTitle>Job Information</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-muted-foreground">
            Job detail implementation coming soon...
          </p>
          <p className="text-sm mt-4">
            This page will show:
          </p>
          <ul className="list-disc list-inside text-sm text-muted-foreground mt-2 space-y-1">
            <li>Job status and metadata</li>
            <li>Extracted fields (partial_fields)</li>
            <li>Entity snapshot (Talent/Project)</li>
            <li>Match results with FB buttons</li>
            <li>Interaction events timeline</li>
          </ul>
        </CardContent>
      </Card>
    </div>
  );
}
