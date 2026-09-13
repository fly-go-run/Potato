import XCTest
@testable import PotatoMobile

final class KeychainTests: XCTestCase {
    func testSignedAppCanStoreUpdateAndRemoveDeviceCredential() throws {
        let account = "test-" + UUID().uuidString
        defer { try? SecureToken.save("", account: account) }
        try SecureToken.save("fixture-first", account: account)
        XCTAssertEqual(SecureToken.read(account: account), "fixture-first")
        try SecureToken.save("fixture-updated", account: account)
        XCTAssertEqual(SecureToken.read(account: account), "fixture-updated")
        try SecureToken.save("", account: account)
        XCTAssertEqual(SecureToken.read(account: account), "")
    }
}
