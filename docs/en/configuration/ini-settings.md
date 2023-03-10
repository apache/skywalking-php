# INI Settings

This is the configuration list supported in `php.ini`.

| Configuration Item                               | Description                                                                                                              | Default Value             |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ | ------------------------- |
| skywalking_agent.enable                          | Enable skywalking_agent extension or not.                                                                                | Off                       |
| skywalking_agent.log_file                        | Log file path.                                                                                                           | /tmp/skywalking-agent.log |
| skywalking_agent.log_level                       | Log level: one of `OFF`, `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`.                                                      | INFO                      |
| skywalking_agent.runtime_dir                     | Skywalking agent runtime directory.                                                                                      | /tmp/skywalking-agent     |
| skywalking_agent.server_addr                     | Address of skywalking oap server.                                                                                        | 127.0.0.1:11800           |
| skywalking_agent.service_name                    | Application service name.                                                                                                | hello-skywalking          |
| skywalking_agent.skywalking_version              | Skywalking version, 8 or 9.                                                                                              | 8                         |
| skywalking_agent.authentication                  | Skywalking authentication token, let it empty if the backend isn't enabled.                                              |                           |
| skywalking_agent.worker_threads                  | Skywalking worker threads, 0 will auto set as the cpu core size.                                                         | 0                         |
| skywalking_agent.enable_tls                      | Wether to enable tls for gPRC, default is false.                                                                         | Off                       |
| skywalking_agent.ssl_trusted_ca_path             | The gRPC SSL trusted ca file.                                                                                            |                           |
| skywalking_agent.ssl_key_path                    | The private key file. Enable mTLS when `ssl_key_path` and `ssl_cert_chain_path` exist.                                   |                           |
| skywalking_agent.ssl_cert_chain_path             | The certificate file. Enable mTLS when `ssl_key_path` and `ssl_cert_chain_path` exist.                                   |                           |
| skywalking_agent.heartbeat_period                | Agent heartbeat report period. Unit, second.                                                                             | 30                        |
| skywalking_agent.properties_report_period_factor | The agent sends the instance properties to the backend every heartbeat_period * properties_report_period_factor seconds. | 10                        |
