use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let is_https = request
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map_or(false, |v| v == "https")
        || request.uri().scheme_str() == Some("https");

    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Prevent MIME sniffing
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    // Prevent clickjacking
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    // No referrer leakage
    headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());
    // Disable unnecessary browser features
    headers.insert(
        "Permissions-Policy",
        "geolocation=(), microphone=(), camera=(), payment=(), usb=(), magnetometer=(), gyroscope=()"
            .parse()
            .unwrap(),
    );
    // XSS filter for older browsers
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    // Block cross-domain policies (Flash/PDF)
    headers.insert("X-Permitted-Cross-Domain-Policies", "none".parse().unwrap());
    // Content Security Policy
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; font-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
            .parse()
            .unwrap(),
    );

    if is_https {
        headers.insert(
            "Strict-Transport-Security",
            "max-age=31536000; includeSubDomains".parse().unwrap(),
        );
    }

    response
}
