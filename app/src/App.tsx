import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

function App() {
  return (
    <main className="bg-background text-foreground flex min-h-screen items-center justify-center p-6">
      <Card className="w-full max-w-xl shadow-sm">
        <CardHeader>
          <CardTitle>Codex Visualization Placeholder</CardTitle>
          <CardDescription>
            `Tauri`, `Tailwind CSS`, `shadcn/ui` 기본 구성이 연결된 상태입니다.
          </CardDescription>
        </CardHeader>
        <CardContent className="text-muted-foreground space-y-4 text-sm">
          <p>지금은 플레이스홀더 앱만 유지하고, 실제 화면 설계는 다음 단계에서 채우면 됩니다.</p>
          <div className="flex flex-wrap gap-2">
            <span className="bg-muted text-muted-foreground rounded-md px-2 py-1">Tauri 2</span>
            <span className="bg-muted text-muted-foreground rounded-md px-2 py-1">Vite</span>
            <span className="bg-muted text-muted-foreground rounded-md px-2 py-1">Tailwind v4</span>
            <span className="bg-muted text-muted-foreground rounded-md px-2 py-1">shadcn/ui</span>
          </div>
        </CardContent>
        <CardFooter className="justify-end gap-2">
          <Button variant="outline">Placeholder</Button>
          <Button>Ready</Button>
        </CardFooter>
      </Card>
    </main>
  );
}

export default App;
