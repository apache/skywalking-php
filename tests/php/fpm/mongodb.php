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

$manager = new MongoDB\Driver\Manager("mongodb://root:example@127.0.0.1:27017");

{
    $command = new MongoDB\Driver\Command(['ping' => 1]);
    $manager->executeCommand('admin', $command);
}

{
    $bulk = new MongoDB\Driver\BulkWrite;
    $bulk->insert(['x' => 1, 'y' => 'foo']);
    $bulk->insert(['x' => 2, 'y' => 'bar']);
    $bulk->insert(['x' => 3, 'y' => 'baz']);
    $manager->executeBulkWrite('my_db.my_collection', $bulk);
}

{
    $query = new MongoDB\Driver\Query(['x' => 1], []);
    $manager->executeQuery('my_db.my_collection', $query);
}

try {
    $manager2 = new MongoDB\Driver\Manager("mongodb://root:example@127.0.0.1:27018,127.0.0.1:27019");
    $command = new MongoDB\Driver\Command(['ping' => 1]);
    $manager2->executeCommand('admin', $command);
} catch(MongoDB\Driver\Exception\ConnectionTimeoutException $e) {
}

echo "ok";
