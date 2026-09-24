import XCTest
import Observation
@testable import PotatoMobile

final class LanguageTests: XCTestCase {
    func testSystemLanguageResolutionAndFallback() {
        XCTAssertEqual(AppLanguage.system.resolved(preferredLanguages: ["en-GB", "zh-Hans"]), .english)
        XCTAssertEqual(AppLanguage.system.resolved(preferredLanguages: ["zh-Hant-TW", "en"]), .simplifiedChinese)
        XCTAssertEqual(AppLanguage.system.resolved(preferredLanguages: ["fr-FR", "zh-CN"]), .simplifiedChinese)
        XCTAssertEqual(AppLanguage.system.resolved(preferredLanguages: ["fr-FR"]), .english)
        XCTAssertEqual(AppLanguage.system.resolved(preferredLanguages: []), .english)
        XCTAssertEqual(AppLanguage.english.resolved(preferredLanguages: ["zh-CN"]), .english)
        XCTAssertEqual(AppLanguage.simplifiedChinese.resolved(preferredLanguages: ["en"]), .simplifiedChinese)
    }
    func testLegacySettingsAndRoundTripPreservePrompt() throws {
        var settings = ConnectionSettings()
        settings.systemPrompt = "Keep my custom reply instructions exactly. 中文原文。"
        var json = try XCTUnwrap(JSONSerialization.jsonObject(with: JSONEncoder().encode(settings)) as? [String: Any])
        json.removeValue(forKey: "language")
        let old = try JSONDecoder().decode(ConnectionSettings.self, from: JSONSerialization.data(withJSONObject: json))
        XCTAssertEqual(old.languageMode, .system)
        for mode in AppLanguage.allCases {
            settings.languageMode = mode
            let restored = try JSONDecoder().decode(ConnectionSettings.self, from: JSONEncoder().encode(settings))
            XCTAssertEqual(restored.languageMode, mode)
            XCTAssertEqual(restored.systemPrompt, settings.systemPrompt)
        }
        XCTAssertEqual(try JSONDecoder().decode(AppLanguage.self, from: Data("\"unsupported\"".utf8)), .system)
    }
    func testTranslationsKeepInterpolatedContentLiteral() {
        XCTAssertEqual(L10n.translate("排队中", language: .english), "Queued")
        XCTAssertEqual(L10n.translate("排队中", language: .simplifiedChinese), "排队中")
        let userText = "用户原文 100% %@"
        XCTAssertEqual(L10n.translate("预览 %@，第 %@ 张，共 %@ 张", arguments: [userText,"2","3"], language: .english), "Preview 用户原文 100% %@, image 2 of 3")
        XCTAssertEqual(L10n.translate("模型与思考，%@", arguments: ["gpt-model"], language: .simplifiedChinese), "模型与思考，gpt-model")
    }
    func testLanguageChangeInvalidatesComputedLabels() {
        let original = AppLocalization.shared.selection
        defer { AppLocalization.shared.selection = original }
        AppLocalization.shared.selection = .simplifiedChinese
        let changed = expectation(description: "Computed localized string is observable")
        withObservationTracking {
            XCTAssertEqual(L10n.tr("设置"), "设置")
        } onChange: { changed.fulfill() }
        AppLocalization.shared.selection = .english
        wait(for: [changed], timeout: 1)
        XCTAssertEqual(L10n.tr("设置"), "Settings")
        XCTAssertEqual(L10n.tr("在“\("用户项目")”项目中开始对话"), "Start a conversation in “用户项目”")
    }
    func testBundledLanguagesHaveMatchingKeysAndPlaceholders() throws {
        func dictionary(_ language: String) throws -> [String: String] {
            let folder = try XCTUnwrap(Bundle.main.path(forResource: language, ofType: "lproj"))
            let data = try Data(contentsOf: URL(fileURLWithPath: folder).appendingPathComponent("Localizable.strings"))
            return try XCTUnwrap(PropertyListSerialization.propertyList(from: data, format: nil) as? [String: String])
        }
        let english = try dictionary("en"), chinese = try dictionary("zh-Hans")
        XCTAssertGreaterThan(english.count, 700)
        XCTAssertEqual(Set(english.keys), Set(chinese.keys))
        for (key, value) in english {
            XCTAssertFalse(value.isEmpty, key)
            XCTAssertEqual(key.components(separatedBy: "%@").count, value.components(separatedBy: "%@").count, key)
        }
    }
}
