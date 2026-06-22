import React, { createContext, useContext, useEffect } from "react";
import { connectWs, disconnectWs } from "../lib/transport";

interface User {
  sub: string;
  email?: string;
  exp?: number;
}

interface AuthContextType {
  user: User | null;
  loading: boolean;
  logout: () => Promise<void>;
  refresh: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const user = { sub: "local-admin", email: "admin@local" };

  useEffect(() => {
    connectWs();
    return () => disconnectWs();
  }, []);

  return (
    <AuthContext.Provider value={{ user, loading: false, logout: async () => {}, refresh: async () => {} }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
