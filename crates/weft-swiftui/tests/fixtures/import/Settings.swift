// A hand-written SwiftUI screen, not printed by `weft swiftui`: what an app team writes. The
// import test reads it into Weft and pins the document and every loss.
import SwiftUI

@Observable
final class SettingsModel {
    var name = ""
    var email = ""
    var notifications = true
    var plan = "free"
    var devices: [Device] = []
    var saving = false
}

struct Device: Identifiable {
    let id: UUID
    var title: String
}

struct DeviceRow: View {
    let device: Device

    var body: some View {
        HStack {
            Image(systemName: "laptopcomputer")
            Text(device.title)
        }
    }
}

struct SettingsView: View {
    @Bindable var model: SettingsModel
    var onSave: () -> Void = {}
    @State private var showHelp = false

    var body: some View {
        NavigationStack {
            Form {
                Section("Profile") {
                    TextField("Name", text: $model.name)
                        .accessibilityIdentifier("name")
                    TextField("Email", text: $model.email)
                        .textContentType(.emailAddress)
                }
                Section("Notifications") {
                    Toggle("Push notifications", isOn: $model.notifications)
                        .tint(.green)
                    Picker("Plan", selection: $model.plan) {
                        Text("Free").tag("free")
                        Text("Pro").tag("pro")
                    }
                }
                Section("Devices") {
                    ForEach(model.devices) { device in
                        DeviceRow(device: device)
                    }
                    ForEach(0..<3) { i in
                        Text("Slot \(i)")
                    }
                }
                if model.devices.count > 2 {
                    Text("You have many devices.")
                        .foregroundStyle(.secondary)
                }
                Button("Save") {
                    model.saving = true
                    onSave()
                }
                .disabled(model.saving)
                .buttonStyle(.borderedProminent)
                Button("Help") { showHelp = true }
            }
            .navigationTitle("Settings")
            .padding(12)
        }
        .sheet(isPresented: $showHelp) {
            Text("Ask support for help.")
            Circle().frame(width: 20, height: 20)
        }
    }
}

#Preview {
    SettingsView(model: SettingsModel())
}
