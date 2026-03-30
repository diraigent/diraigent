import Foundation
import Security

/// Keychain CRUD wrapper using the Security framework.
///
/// Tokens are stored with `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
/// so they are never included in backups or synced to other devices.
enum KeychainHelper {
    private static let service = "at.faua.diraigent"

    /// Save data to the keychain for the given key.
    static func save(key: String, data: Data) {
        delete(key: key)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
        ]
        SecItemAdd(query as CFDictionary, nil)
    }

    /// Read data from the keychain for the given key.
    static func read(key: String) -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess else {
            return nil
        }
        return result as? Data
    }

    /// Delete an entry from the keychain.
    static func delete(key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
        ]
        SecItemDelete(query as CFDictionary)
    }
}

// MARK: - String convenience

extension KeychainHelper {
    /// Save a string value to the keychain.
    static func saveString(key: String, value: String) {
        guard let data = value.data(using: .utf8) else { return }
        save(key: key, data: data)
    }

    /// Read a string value from the keychain.
    static func readString(key: String) -> String? {
        guard let data = read(key: key) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}
