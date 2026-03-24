import SwiftUI

/// Detail view for a work item showing description, success criteria, and linked tasks.
struct WorkDetailView: View {
    @Environment(AppState.self) private var appState

    let work: Work

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: DiraigentTheme.spacingLG) {
                // Header
                headerSection

                Divider()

                // Description
                if let description = work.description, !description.isEmpty {
                    descriptionSection(description)
                }

                // Success criteria
                if let criteria = work.successCriteria, !criteria.isEmpty {
                    successCriteriaSection(criteria)
                }

                // Metadata
                metadataSection
            }
            .padding()
        }
        .navigationTitle(work.title)
        .navigationBarTitleDisplayMode(.inline)
    }

    // MARK: - Sections

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingMD) {
            Text(work.title)
                .font(DiraigentTheme.titleFont)

            HStack(spacing: DiraigentTheme.spacingSM) {
                if let kind = work.workType {
                    WorkKindBadge(kind: kind)
                }
                if let status = work.status {
                    WorkStatusBadge(status: status)
                }
                if let priority = work.priority {
                    PriorityIndicator(priority: priority)
                }
            }
        }
    }

    private func descriptionSection(_ description: String) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Label("Description", systemImage: "text.alignleft")
                .font(DiraigentTheme.headlineFont)

            Text(description)
                .font(DiraigentTheme.bodyFont)
                .foregroundStyle(.secondary)
        }
    }

    private func successCriteriaSection(_ criteria: [String: AnyCodable]) -> some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Label("Success Criteria", systemImage: "checkmark.circle")
                .font(DiraigentTheme.headlineFont)

            ForEach(Array(criteria.keys.sorted()), id: \.self) { key in
                HStack(alignment: .top, spacing: DiraigentTheme.spacingSM) {
                    Image(systemName: "circle")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .padding(.top, 3)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(key)
                            .font(DiraigentTheme.bodyFont)
                        if let value = criteria[key] {
                            Text(String(describing: value.value))
                                .font(DiraigentTheme.captionFont)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
        }
    }

    private var metadataSection: some View {
        VStack(alignment: .leading, spacing: DiraigentTheme.spacingSM) {
            Label("Details", systemImage: "info.circle")
                .font(DiraigentTheme.headlineFont)

            LazyVGrid(columns: [
                GridItem(.flexible()),
                GridItem(.flexible()),
            ], spacing: DiraigentTheme.spacingSM) {
                if let created = work.createdAt {
                    MetadataItem(label: "Created", value: formatDate(created))
                }
                if let updated = work.updatedAt {
                    MetadataItem(label: "Updated", value: formatDate(updated))
                }
                if let priority = work.priority {
                    MetadataItem(label: "Priority", value: "\(priority)")
                }
                if work.autoStatus == true {
                    MetadataItem(label: "Auto Status", value: "Enabled")
                }
            }
        }
    }

    // MARK: - Helpers

    private func formatDate(_ isoString: String) -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        guard let date = formatter.date(from: isoString) else {
            formatter.formatOptions = [.withInternetDateTime]
            guard let date = formatter.date(from: isoString) else { return isoString }
            return displayFormat(date)
        }
        return displayFormat(date)
    }

    private func displayFormat(_ date: Date) -> String {
        let display = DateFormatter()
        display.dateStyle = .medium
        display.timeStyle = .short
        return display.string(from: date)
    }
}

/// A labeled metadata item for detail views.
struct MetadataItem: View {
    let label: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.subheadline)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
