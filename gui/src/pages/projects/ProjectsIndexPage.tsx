import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Link } from "react-router-dom";
import { useI18n } from "@/lib/i18n";

export function ProjectsIndexPage() {
  const { t } = useI18n();

  return (
    <div className="space-y-4">
      <div className="space-y-1">
        <h1 className="text-2xl font-bold">{t("projectDetail.index.title")}</h1>
        <p className="text-muted-foreground">{t("projectDetail.index.description")}</p>
      </div>
      <Card>
        <CardHeader>
          <CardTitle>{t("projectDetail.index.cardTitle")}</CardTitle>
          <CardDescription>{t("projectDetail.index.cardDescription")}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div className="text-sm text-muted-foreground">
            {t("projectDetail.index.helper")}
          </div>
          <div className="flex flex-wrap gap-2">
            <Button asChild variant="outline">
              <Link to="/queue">{t("projectDetail.index.goToQueue")}</Link>
            </Button>
            <Button asChild>
              <Link to="/projects/1">{t("projectDetail.index.sampleProject")}</Link>
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
