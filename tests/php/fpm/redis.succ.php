<?php

// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


use Webmozart\Assert\Assert;

require_once dirname(__DIR__) . "/vendor/autoload.php";

{
    $client = new Redis();
    $client->connect("127.0.0.1", 6379);
    $client->auth('password');
    $client->mSet(['key0' => 'value0', 'key1' => 'value1']);
    Assert::same($client->get('key0'), 'value0');
    Assert::same($client->get('key1'), 'value1');
}

echo "ok";
