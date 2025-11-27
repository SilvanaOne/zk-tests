import NextAuth, { DefaultSession, DefaultJWT } from "next-auth";

declare module "next-auth" {
  interface Session {
    accessToken?: string;
    refreshToken?: string;
    idToken?: string; // OAuth ID JWT token (Google, GitHub, etc.)
    provider?: string;
    userId?: string;
    user: {
      id?: string;
    } & DefaultSession["user"];
  }

  interface JWT extends DefaultJWT {
    accessToken?: string;
    refreshToken?: string;
    idToken?: string; // OAuth ID JWT token (Google, GitHub, etc.)
    provider?: string;
    userId?: string;
  }
}
