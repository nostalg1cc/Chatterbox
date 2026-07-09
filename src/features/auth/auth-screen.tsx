import { useState } from "react";
import { toast } from "sonner";
import { Loader2Icon, MailIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { supabase } from "@/lib/supabase";

const USERNAME_RE = /^[a-z0-9_]{3,20}$/;

export function AuthScreen() {
  const [pendingEmail, setPendingEmail] = useState<string | null>(null);

  return (
    <div className="flex h-full items-center justify-center p-6">
      <Card className="w-full max-w-sm">
        {pendingEmail ? (
          <VerifyNotice email={pendingEmail} onBack={() => setPendingEmail(null)} />
        ) : (
          <>
            <CardHeader>
              <CardTitle className="tracking-[0.25em] uppercase">Dislight</CardTitle>
              <CardDescription>Quiet, one-on-one conversations.</CardDescription>
            </CardHeader>
            <CardContent>
              <Tabs defaultValue="login">
                <TabsList className="w-full">
                  <TabsTrigger value="login">Log in</TabsTrigger>
                  <TabsTrigger value="signup">Sign up</TabsTrigger>
                </TabsList>
                <TabsContent value="login" className="pt-2">
                  <LoginForm />
                </TabsContent>
                <TabsContent value="signup" className="pt-2">
                  <SignupForm onNeedsVerification={setPendingEmail} />
                </TabsContent>
              </Tabs>
            </CardContent>
          </>
        )}
      </Card>
    </div>
  );
}

function LoginForm() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email || !password) return;
    setLoading(true);
    const { error } = await supabase.auth.signInWithPassword({ email, password });
    setLoading(false);
    if (error) {
      toast.error(
        error.message === "Invalid login credentials"
          ? "Wrong email or password."
          : error.message
      );
    }
  };

  return (
    <form onSubmit={submit} className="flex flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="login-email">Email</Label>
        <Input
          id="login-email"
          type="email"
          autoComplete="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          required
        />
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="login-password">Password</Label>
        <Input
          id="login-password"
          type="password"
          autoComplete="current-password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          required
        />
      </div>
      <Button type="submit" disabled={loading} className="mt-1">
        {loading && <Loader2Icon className="animate-spin" />}
        Log in
      </Button>
    </form>
  );
}

function SignupForm({
  onNeedsVerification,
}: {
  onNeedsVerification: (email: string) => void;
}) {
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [usernameStatus, setUsernameStatus] = useState<
    "idle" | "checking" | "available" | "taken" | "invalid"
  >("idle");

  const checkUsername = async (value: string) => {
    if (!value) {
      setUsernameStatus("idle");
      return;
    }
    if (!USERNAME_RE.test(value)) {
      setUsernameStatus("invalid");
      return;
    }
    setUsernameStatus("checking");
    const { data, error } = await supabase.rpc("username_available", {
      check_username: value,
    });
    if (error) {
      setUsernameStatus("idle");
      return;
    }
    setUsernameStatus(data ? "available" : "taken");
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const name = username.trim().toLowerCase();
    if (!USERNAME_RE.test(name)) {
      toast.error("Usernames are 3–20 characters: a–z, 0–9, underscores.");
      return;
    }
    if (password.length < 8) {
      toast.error("Password needs at least 8 characters.");
      return;
    }
    setLoading(true);
    const { data, error } = await supabase.auth.signUp({
      email,
      password,
      options: {
        data: {
          username: name,
          display_name: displayName.trim() || name,
        },
      },
    });
    setLoading(false);
    if (error) {
      toast.error(
        error.message.includes("Database error")
          ? "That username or email is already taken."
          : error.message
      );
      return;
    }
    // No session means email confirmation is on — show the verify notice.
    if (!data.session) onNeedsVerification(email);
  };

  return (
    <form onSubmit={submit} className="flex flex-col gap-4">
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="signup-username">Username</Label>
        <Input
          id="signup-username"
          autoComplete="off"
          spellCheck={false}
          value={username}
          onChange={(e) => setUsername(e.target.value.toLowerCase())}
          onBlur={() => void checkUsername(username.trim())}
          placeholder="lowercase, 3–20 chars"
          required
        />
        {usernameStatus !== "idle" && (
          <p
            className={
              usernameStatus === "taken" || usernameStatus === "invalid"
                ? "text-xs text-destructive"
                : "text-xs text-muted-foreground"
            }
          >
            {usernameStatus === "checking" && "Checking…"}
            {usernameStatus === "available" && "Available."}
            {usernameStatus === "taken" && "Already taken."}
            {usernameStatus === "invalid" && "Only a–z, 0–9 and _, 3–20 characters."}
          </p>
        )}
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="signup-displayname">Display name</Label>
        <Input
          id="signup-displayname"
          autoComplete="off"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
          placeholder="How friends see you"
          maxLength={50}
        />
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="signup-email">Email</Label>
        <Input
          id="signup-email"
          type="email"
          autoComplete="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          required
        />
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="signup-password">Password</Label>
        <Input
          id="signup-password"
          type="password"
          autoComplete="new-password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="8+ characters"
          required
        />
      </div>
      <Button type="submit" disabled={loading || usernameStatus === "taken"} className="mt-1">
        {loading && <Loader2Icon className="animate-spin" />}
        Create account
      </Button>
    </form>
  );
}

function VerifyNotice({ email, onBack }: { email: string; onBack: () => void }) {
  const [resending, setResending] = useState(false);

  const resend = async () => {
    setResending(true);
    const { error } = await supabase.auth.resend({ type: "signup", email });
    setResending(false);
    if (error) toast.error("Couldn't resend the email.");
    else toast.success("Verification email sent again.");
  };

  return (
    <>
      <CardHeader>
        <CardTitle>Check your inbox</CardTitle>
        <CardDescription>Then come back and log in.</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <Alert>
          <MailIcon />
          <AlertTitle>Verification link sent</AlertTitle>
          <AlertDescription>
            We sent a confirmation link to {email}. Click it to activate your account.
          </AlertDescription>
        </Alert>
        <div className="flex gap-2">
          <Button variant="outline" className="flex-1" onClick={onBack}>
            Back to login
          </Button>
          <Button variant="secondary" className="flex-1" onClick={resend} disabled={resending}>
            {resending && <Loader2Icon className="animate-spin" />}
            Resend email
          </Button>
        </div>
      </CardContent>
    </>
  );
}
