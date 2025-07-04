import NextAuth from "next-auth";
import Google from "next-auth/providers/google";
import GitHub from "next-auth/providers/github";

export const { handlers, signIn, signOut, auth } = NextAuth({
  providers: [
    Google({
      clientId: process.env.GOOGLE_CLIENT_ID!,
      clientSecret: process.env.GOOGLE_CLIENT_SECRET!,
    }),
    GitHub({
      clientId: process.env.GITHUB_CLIENT_ID!,
      clientSecret: process.env.GITHUB_CLIENT_SECRET!,
    }),
  ],
  callbacks: {
    jwt({ token, user, account }) {
      // Persist the OAuth access_token, refresh_token, and ID token to the token right after signin
      if (account) {
        token.accessToken = account.access_token;
        token.refreshToken = account.refresh_token;
        token.idToken = account.id_token;
        token.provider = account.provider;
      }
      // Add user info to token
      if (user) {
        token.userId = user.id;
      }
      return token;
    },
    session({ session, token }) {
      // Send properties to the client
      if (token) {
        session.accessToken = token.accessToken as string;
        session.refreshToken = token.refreshToken as string;
        session.idToken = token.idToken as string;
        session.provider = token.provider as string;
        session.userId = token.userId as string;
      }
      return session;
    },
  },
});

// For local development, your redirect URI should be:
// http://localhost:3001/api/auth/callback/google
