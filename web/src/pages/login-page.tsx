import { useSearchParams, Navigate } from "react-router-dom";
import { useAuth } from "@/context/AuthContext";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { LogIn, ShieldAlert, ShieldCheck } from "lucide-react";

export function LoginPage() {
  const { user, login, loading } = useAuth();
  const [searchParams] = useSearchParams();
  const error = searchParams.get("error");

  if (user && !loading) {
    return <Navigate to="/" replace />;
  }

  const getErrorMessage = (err: string) => {
    switch (err) {
      case "not_authorized":
        return "Your account is not on the admin whitelist.";
      case "auth_not_configured":
        return "Google OAuth is not configured on the server.";
      case "token_exchange_failed":
        return "Failed to exchange code for token with Google.";
      default:
        return "An unexpected authentication error occurred.";
    }
  };

  return (
    <div className="min-h-screen bg-black flex items-center justify-center p-4">
      <div className="w-full max-w-md space-y-8">
        <div className="text-center">
          <div className="flex justify-center mb-6">
            <div className="p-4 bg-zinc-900 rounded-full border border-zinc-800">
              <ShieldCheck className="w-12 h-12 text-zinc-100" />
            </div>
          </div>
          <h1 className="text-4xl font-bold tracking-tight text-white mb-2">SHABAKAT</h1>
          <p className="text-zinc-500 font-medium">ADMINISTRATIVE GATEWAY</p>
        </div>

        <Card className="bg-zinc-950 border-zinc-800">
          <CardHeader>
            <CardTitle className="text-zinc-100">Authentication Required</CardTitle>
            <CardDescription className="text-zinc-500">
              Please sign in with your Google administrator account to access the configuration panels.
            </CardDescription>
          </CardHeader>
          <CardContent>
            {error && (
              <div className="mb-6 p-4 bg-red-950/30 border border-red-900/50 rounded-lg flex items-start gap-3">
                <ShieldAlert className="w-5 h-5 text-red-500 shrink-0 mt-0.5" />
                <div className="text-sm text-red-200">
                  <p className="font-semibold">Access Denied</p>
                  <p>{getErrorMessage(error)}</p>
                </div>
              </div>
            )}

            <Button
              onClick={login}
              disabled={loading}
              className="w-full bg-white text-black hover:bg-zinc-200 h-12 text-lg font-semibold transition-all"
            >
              <LogIn className="w-5 h-5 mr-2" />
              Sign in with Google
            </Button>
          </CardContent>
        </Card>

        <div className="text-center">
          <p className="text-xs text-zinc-700 uppercase tracking-widest">
            Enterprise Security Tier &bull; System Locked
          </p>
        </div>
      </div>
    </div>
  );
}
