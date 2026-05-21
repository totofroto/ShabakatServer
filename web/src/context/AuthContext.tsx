import React, { createContext, useContext, useEffect, useState } from "react";

interface User {
  sub: string;
  email: string;
  exp: number;
}

interface AuthContextType {
  user: User | null;
  loading: boolean;
  login: () => void;
  logout: () => Promise<void>;
  refresh: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  // DEBUG: hardcoded debug user — remove before production deploy
  const [user] = useState<User | null>({ sub: "debug", email: "debug@local", exp: 9999999999 });
  const [loading] = useState(false);

  const refresh = async () => {
    // DEBUG: no-op while auth is disabled
  };

  useEffect(() => {
    // DEBUG: skip API call while auth is disabled
  }, []);

  const login = () => {
    window.location.href = "/api/auth/google/login";
  };

  const logout = async () => {
    // DEBUG: no-op while auth is disabled
  };

  return (
    <AuthContext.Provider value={{ user, loading, login, logout, refresh }}>
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
