# Licensed to the Apache Software Foundation (ASF) under one or more
# contributor license agreements.  See the NOTICE file distributed with
# this work for additional information regarding copyright ownership.
# The ASF licenses this file to You under the Apache License, Version 2.0
# (the "License"); you may not use this file except in compliance with
# the License.  You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

Vagrant.configure("2") do |config|
    config.vm.box = "generic/alpine316"
    config.vm.box_version = "4.1.10"
    config.vm.box_check_update = false

    config.vm.network "forwarded_port", guest: 19876, host: 19876
    config.vm.network "forwarded_port", guest: 12800, host: 12800
    config.vm.network "forwarded_port", guest: 3306, host: 3306
    config.vm.network "forwarded_port", guest: 6379, host: 6379
    config.vm.network "forwarded_port", guest: 11211, host: 11211
    config.vm.network "forwarded_port", guest: 5672, host: 5672

    config.vm.synced_folder ".", "/vagrant"

    config.vm.provision "shell", inline: <<-SHELL
        apk add --no-cache docker docker-cli-compose
        service docker restart
        sleep 3
        cd /vagrant
        sudo docker compose up -d
    SHELL
end
