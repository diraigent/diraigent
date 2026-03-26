import Foundation
import CryptoKit

/// Static helper for the PKCE (Proof Key for Code Exchange) flow.
///
/// Generates a random code verifier and derives the corresponding
/// SHA-256 code challenge, both in base64url encoding.
enum PKCEHelper {
    /// Generate a 32-byte random base64url-encoded code verifier.
    static func generateCodeVerifier() -> String {
        var buffer = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, buffer.count, &buffer)
        return Data(buffer)
            .base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    /// Derive the S256 code challenge for a given verifier.
    ///
    /// Returns the SHA-256 hash of the verifier, base64url-encoded.
    static func codeChallenge(for verifier: String) -> String {
        guard let data = verifier.data(using: .utf8) else { return verifier }
        let hash = SHA256.hash(data: data)
        return Data(hash)
            .base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
