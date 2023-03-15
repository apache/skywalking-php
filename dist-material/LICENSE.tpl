{{ .LicenseContent }}

=======================================================================
Apache SkyWalking Subcomponents:

The Apache SkyWalking project contains subcomponents with separate copyright
notices and license terms. Your use of the source code for the these
subcomponents is subject to the terms and conditions of the following
licenses.
========================================================================

{{ range .Groups }}
========================================================================
{{ .LicenseID }} licenses
========================================================================
The following components are provided under the {{ .LicenseID }} License. See project link for details.
{{- if contains .LicenseID "Apache-2.0" }}
The text of each license is the standard Apache 2.0 license.
{{- else }}
The text of each license is also included in licenses/LICENSE-[project].txt.
{{ end }}

    {{- range .Deps }}
    https://crates.io/crates/{{ .Name }}/{{ .Version }} {{ .Version }} {{ .LicenseID }}
    {{- end }}
{{ end }}
