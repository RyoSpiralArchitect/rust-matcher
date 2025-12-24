import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { ThumbsUp, ThumbsDown } from "lucide-react"

function App() {
  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-4xl mx-auto space-y-6">
        <h1 className="text-3xl font-bold">SR Matcher GUI</h1>
        <p className="text-muted-foreground">
          マッチング候補の確認とフィードバック
        </p>

        {/* サンプル候補カード */}
        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <div>
                <CardTitle>山田 太郎</CardTitle>
                <CardDescription>React / TypeScript / AWS</CardDescription>
              </div>
              <Badge variant="secondary">スコア: 0.85</Badge>
            </div>
          </CardHeader>
          <CardContent>
            <div className="flex items-center justify-between">
              <div className="text-sm text-muted-foreground">
                希望単価: 70万〜 / 東京都 / 即日可能
              </div>
              <div className="flex gap-2">
                <Button variant="outline" size="sm">
                  <ThumbsUp className="h-4 w-4 mr-1" />
                  Good
                </Button>
                <Button variant="outline" size="sm">
                  <ThumbsDown className="h-4 w-4 mr-1" />
                  NG
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <div className="flex items-center justify-between">
              <div>
                <CardTitle>佐藤 花子</CardTitle>
                <CardDescription>Python / Django / GCP</CardDescription>
              </div>
              <Badge variant="secondary">スコア: 0.72</Badge>
            </div>
          </CardHeader>
          <CardContent>
            <div className="flex items-center justify-between">
              <div className="text-sm text-muted-foreground">
                希望単価: 65万〜 / 神奈川県 / 来月〜
              </div>
              <div className="flex gap-2">
                <Button variant="outline" size="sm">
                  <ThumbsUp className="h-4 w-4 mr-1" />
                  Good
                </Button>
                <Button variant="outline" size="sm">
                  <ThumbsDown className="h-4 w-4 mr-1" />
                  NG
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

export default App
