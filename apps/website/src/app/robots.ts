import type { MetadataRoute } from "next";
import { APP_URL } from "~/meta";

export default function robots(): MetadataRoute.Robots {
  return {
    rules: {
      userAgent: "*",
      allow: "/",
    },
    sitemap: APP_URL + "/sitemap.xml",
  };
}
