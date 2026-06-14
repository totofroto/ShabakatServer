import React, { createContext, useContext, useEffect, useState } from "react";
import { transport } from "../lib/transport";

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
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = async () => {
    try {
      const res = await transport.fetch("/api/auth/me");
      if (res.ok) {
        const data = await res.json();
        setUser(data);
      } else if (res.status === 401) {
        setUser(null);
      }
    } catch (e) {
      console.error("Auth check failed", e);
      setUser(null);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    // Instantly register user as authenticated by setting a mock token if not present
    if (!localStorage.getItem("shabakat_session_token")) {
      localStorage.setItem("shabakat_session_token", "mock-admin-token");
    }

    // Check for token in URL hash (passed from google_callback)
    const hash = window.location.hash;
    if (hash.startsWith("#token=")) {
      const token = hash.substring(7);
      if (token) {
        localStorage.setItem("shabakat_session_token", token);
        // Clean up URL hash without refreshing
        window.history.replaceState(null, "", window.location.pathname + window.location.search);
      }
    }
    refresh();
  }, []);

  const login = () => {
    // Set a mock token and immediately authenticate user locally without credentials
    localStorage.setItem("shabakat_session_token", "mock-admin-token");
    setUser({
      sub: "local-admin",
      email: "admin@shabakat.local",
      exp: Math.floor(Date.now() / 1000) + 7 * 24 * 3600,
    });
  };

  const logout = async () => {
    try {
      localStorage.removeItem("shabakat_session_token");
      await transport.fetch("/api/auth/logout", { method: "POST" });
      setUser(null);
      window.location.href = "/login";
    } catch (e) {
      console.error("Logout failed", e);
    }
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
