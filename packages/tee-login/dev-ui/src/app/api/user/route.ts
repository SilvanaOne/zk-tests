import { auth } from "@/lib/auth";
import { NextResponse } from "next/server";

export async function GET() {
  const session = await auth();

  if (!session) {
    return NextResponse.json({ error: "Not authenticated" }, { status: 401 });
  }

  return NextResponse.json({
    user: {
      name: session.user?.name,
      email: session.user?.email,
      image: session.user?.image,
    },
    // The entire session object contains JWT information
    session: session,
    // Note: In NextAuth v5, the JWT is handled internally
    // You can access session data but the raw JWT is abstracted
  });
}
