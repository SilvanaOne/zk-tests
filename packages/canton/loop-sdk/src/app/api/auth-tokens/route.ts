import { auth } from "@/lib/auth";
import { NextResponse } from "next/server";
import type { Session } from "next-auth";

export async function GET() {
  const session = (await auth()) as Session & {
    accessToken?: string;
    provider?: string;
  };

  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  if (!session?.accessToken) {
    return NextResponse.json(
      { error: "No access token available" },
      { status: 400 }
    );
  }

  // This is the data your Rust backend needs
  return NextResponse.json({
    access_token: session.accessToken,
    email: session.user?.email,
    name: session.user?.name,
    provider: session.provider || "google",
    // Add any other user data you need
  });
}
